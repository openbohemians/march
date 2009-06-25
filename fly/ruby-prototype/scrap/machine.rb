#!/usr/bin/env ruby

require 'rbconfig'
require 'fly/error'
require 'fly/program'

=begin
From:   James Edward Gray II - view profile
Date:   Thurs, Sep 30 2004 11:42 pm
Email:    James Edward Gray II <j...@grayproductions.net>
Groups:     comp.lang.ruby
Not yet rated
Rating:
show options

Reply to Author | Forward | Print | Individual Message | Show original | Report Abuse | Find messages by this author

On Sep 30, 2004, at 5:55 PM, Andreas Schwarz wrote:

> Hello,

> it's really nice, however there is one problem: if a define contains a
> syntax error, the calculator quits instead of just showing the error
> message.

> Andreas

I noticed that about the same time you posted this.  It's fixed in the
version below.

Did I mention it has display modes?  Type "bin", or "hex".  :)

I know, it's a toy.  A fun one though...

James Edward Gray II
=end

$DEBUG = true

# Load core components

CORE_COMPONENTS = [
  'stack', 'math', 'console'
]

CORE = "\n\n# CORE COMPONENTS\n"
core_dir = File.join( Config::CONFIG['datadir'], 'fly', 'core' )
CORE_COMPONENTS.each { |name|
  next if name == '.' or name == '..'
  text = File.read( File.join(core_dir, name) )
  CORE << "\n" << text.strip.gsub(/[%]YAML\s+1[.]0/, '').gsub('---', '').strip
  CORE << "\n\n" << "<<: *#{name}" << "\n"
}


module Fly

  class Machine

    def initialize( io )
      # Build user program with core components
      str = "%YAML 1.1\n"
      str << "---"
      str << CORE
      str << "\n\n# USER PROGRAM\n\n"
      str << io.gets(nil).strip.gsub(/[%]YAML\s+1[.]0/, '').gsub('---', '').strip
      str << "\n\n"
      # Display if debug mode
      #puts str if $DEBUG
      # Load yaml
      @code = YAML::load( str )
    end

    def run
      prg = Progam.new( @code )
      prg.main
      #rpn.calc( @code['main'], [ @code ] )
    end

  end


  # class for generating Calculator objects

  class RPNCalc

    #COMPONENTS = [ Core, Math, Console ]
    #include *COMPONENTS

    attr :stack
    attr :ops

    attr_accessor :mode             # defines methods mode() and mode = ...

    # handles setup after constructor
    def initialize( mode = "dec", stack = [ ] )
      @mode = mode
      @stack = stack
      @ops = { }
    end

    ### Operator definition methods ###

    # adds Ruby code as new operator
    def define_op( op, &code )
      @ops[op] = code
    end

    ### Input/Output methods ###

    # parses and executes expressions in postfix notation
    # also understands "def OP RUBY_PROC_CODE" on it's own line
    def calc( postfix_exp, yaml_stack=[] )

      if postfix_exp =~ /^\s*def\s+(\S+)\s+(\{.+\})\s*$/      # define new op
        define_op( $1.downcase, &eval( "proc #{$2}" ) )

      else    # ... or process terms
        terms = postfix_exp.downcase.split(" ")

        terms.each do |t|

          if @ops.include? t      # use custom op definition
            if @ops[t].arity == 2   # choose handler by proc args
              binary &@ops[t]
            else
              unary &@ops[t]
            end
            next
          end

#           COMPONENTS.each do |component|
#             if component.words.key? t
#               send( component.words[t][:name] )
#             end
#           end

          case t

          #when "eval"
          #  calc( top, yaml_stack )

          when /^-?(?:\d[,_\d]*|0[,_0-7]+|0x[,_0-9a-f]+|0b[,_01]+)$/,
               /^-?\d[,_\d]*\.\d[,_\d]*(?:e-?[,_\d]+)?$/
            push( eval( t.tr(",", "") ) )

          else
            # look up the yaml stack heirarchy
            dotchain = (t =~ /\.+/) ? t : t.split('.')
            current = yaml_stack
            dotchain.each { |key|
              v = current.find{ |x| x.key?(key) }
              current = v ? v[key] : nil
              break if current.nil?
            }
            raise "Invalid term: #{t}." unless current
p current
            case current
            when RubyFn
              eval current.code
            else
              push current
            end
          end

        end
      end

      return stack.first #top
    end

#     def method_missing( sym, *args )
#       calc( sym )
#     end


    # primary output method, display stack with optional limit
    def to_s( limit = @stack.size )
      if @stack.size == 0
        return "Empty Stack\n"
      else
        case @mode      # support four types of numerical display
        when "bin"
          return (0...limit).to_a.reverse.inject("") do |str, i|
                  str + "#{i}: #{'%b' % @stack[i]}\n"
          end
        when "dec"
          return (0...limit).to_a.reverse.inject("") do |str, i|
                  str + "#{i}: #{@stack[i]}\n"
          end
        when "hex"
          return (0...limit).to_a.reverse.inject("") do |str, i|
                  str + "#{i}: #{'%x' % @stack[i]}\n"
          end
        when "oct"
          return (0...limit).to_a.reverse.inject("") do |str, i|
                  str + "#{i}: #{'%o' % @stack[i]}\n"
          end
        end
      end
    end

    private

    ### Operator processing methods ###

    # primary input method, adds something to stack
    def push( obj )
      stack.unshift( obj )
    end

    def check( n=1 )
      if @stack.size < n
        raise StackEmptyError, "Insufficient elements on stack for operation."
      end
    end

    def binary
      a,b = drop,drop
      return [b, a]
    end

    #def unary( &op )
    #  if @stack.size < 1
    #    raise "Insufficient elements on stack for operation."
    #  end
    #  return push( op.call( @stack.shift ) )
    #end

    #def binary( &op )
    #  if @stack.size < 2
    #    raise "Insufficient elements on stack for operation."
    #  end
    #  right, left = @stack.slice!(0, 2)
    #  return push( op.call( left, right ) )
    #end

  end

end



#     ### Basis math methods ###
#
#     def add( ) return binary { |l, r| l + r } end
#     def sub( ) return binary { |l, r| l - r } end
#     def mul( ) return binary { |l, r| l * r } end
#     def mod( ) return binary { |l, r| l % r } end
#     def pow( ) return binary { |l, r| l ** r } end
#     def div( )
#       return binary do |l, r|
#         # defeat Ruby's integer division
#         if l.integer? and r.integer? and l % r != 0
#           l / r.to_f
#         else
#           l / r
#         end
#       end
#     end
# 
#     ### To int methods ###
# 
#     def floor( ) return unary { |num| num.floor } end
#     def ceil( )  return unary { |num| num.ceil } end
#     def round( ) return unary { |num| num.round } end
# 
#     ### Higher math methods ###
# 
#     def abs( )   return unary { |num| num.abs } end
#     def sqrt( )  return unary { |num| Math.sqrt( num ) } end
#     def exp( )   return unary { |num| Math.exp( num ) } end
#     def log( )   return unary { |num| Math.log( num ) } end
#     def log10( ) return unary { |num| Math.log10( num ) } end
#     def sin( )   return unary { |num| Math.sin( num ) } end
#     def cos( )   return unary { |num| Math.cos( num ) } end
#     def tan( )   return unary { |num| Math.tan( num ) } end
#     def sinh( )  return unary { |num| Math.sinh( num ) } end
#     def cosh( )  return unary { |num| Math.cosh( num ) } end
#     def tanh( )  return unary { |num| Math.tanh( num ) } end
#     def asin( )  return unary { |num| Math.asin( num ) } end
#     def acos( )  return unary { |num| Math.acos( num ) } end
#     def atan( )  return unary { |num| Math.atan( num ) } end
#     def asinh( ) return unary { |num| Math.asinh( num ) } end
#     def acosh( ) return unary { |num| Math.acosh( num ) } end
#     def atanh( ) return unary { |num| Math.atanh( num ) } end
#     def atan2( ) return binary { |l, r| Math.atan2( l, r ) } end
#
#     ### Bitewise manipulation methods ###
#
#     # Warning:  These methods convert their operands to ints before operation
#     def bit_and( ) return binary { |l, r| l.to_i & r.to_i } end
#     def bit_or( )  return binary { |l, r| l.to_i | r.to_i } end
#     def bit_xor( ) return binary { |l, r| l.to_i ^ r.to_i } end
#     def bit_neg( ) return unary { |num| ~num.to_i } end
