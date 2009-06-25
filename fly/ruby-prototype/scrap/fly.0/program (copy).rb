#!/usr/bin/env ruby

require 'calibre/functor'
#require 'calibre/overload'

require 'fly/word'

# Thanks goes to James Edward Gray II for RPNCalc which got me started on a prototype.

# TODO Allow anchors to work as global variables. How?
#
# TODO Type definitions (Partially implemented)
#
# TODO Implement polymorphic word interface.


module Fly

  CORE_DIR = File.join( Config::CONFIG['datadir'], 'fly', 'core' )
  CORE_PARTS = [ 'math', 'console']

  module Compiler

    def load( io )
      case io
      when IO
        compile( io.read )
      else
        compile( io )
      end
    end

    # Load core components

    def load_kernel
      core = ""
      core << yaml_clean( File.read( File.join(CORE_DIR, 'kernel') ) )
      CORE_PARTS.each { |name|
        str = File.read( File.join(CORE_DIR, name) )
        core << "\n\n" << yaml_clean( str.strip )
      }
      return core
    end

    def yaml_clean( str )
        str.gsub(/\%YAML\s+\d+[.]\d+/, '').gsub('---', '').strip
      end

    # Program compile is a two stage process.

    def compile( code )
      code = compile_kernel( code )
      compile_types( YAML.load( code ) )
      compile_words( YAML.load( code ) )
    end

    def compile_kernel( code )
      core = load_kernel
      # Build user program with core components
      str = "---\n"
      str << core
      str << "\n\n# USER PROGRAM\n\n"
      str << yaml_clean( code.strip )
      str << "\n\n"
      # Display if debug mode
      #puts str if $DEBUG
      # Load yaml
      str
    end

    # Precompile sets up all type definitions.

    def compile_types( yml, prekey=nil )
      yml.each { |key, val|
        current_key = prekey ? "#{prekey}.#{key}" : key
        case val
        when Type
          val.compile( current_key )
        when Hash
          compile_types( val, current_key  )
        end
      }
    end

    # Compile all the words.

    def compile_words( yml, prg=nil )

#       prg ||= Program.new
#       sig = (class << prg; self; end)

      prg ||= Program
      sig = prg
      #sig = (class << prg; self; end)

      yml.each { |key, node|
        case node
        when Word #RubyFn
          name, face = node.compile( key )

          prg.words[name] ||= {}
          prg.words[name][face] ||= node

          prg.faces[name] ||= []
          prg.faces[name] << face
          prg.faces[name].sort! { |a,b| a.size <=> b.size }

          sig.class_eval {
            unless method_defined?( name )
              define_method( name ) do
                hit = []
                prg.faces[name].each do |fac|
                  #cmp = stack[0...fac.size].collect{ |x| x.class }
                  #cmp.size.times do |i|
                  fac.size.times do |i|
                    next unless stack[i].class <= fac[i]
                    hit = fac; break
                  end
                end
                if prg.words[name][hit]
                  prg.words[name][hit].calc(self, *stack[0...hit.size])
                else
                  raise NoMethodError, "word undefined -- #{name}"
                end
                #node.calc(self)
              end
            end
          }
        #when Fn
        #  val.compile( key )
        #  sig.class_eval {
        #    #define_method( key ) { calc( val.code ) }
        #    define_method( key ) { val.calc(self) }
        #  }
        when Hash
          sig.class_eval {
            #define_method( key ) { Program.compile( val ) }
            #ftr = Functor.new( prg ) { |op, obj, *args| obj.send(op, *args) }
            #ftr = SubProgram.new { |op, *args| prg.send(op, *args) }
            ftr = Class.new(sig) #{ |op, *args| prg.send(op, *args) }
            res = compile_words( node, ftr )
            define_method( key ) { ftr }
          }
        when Type
          # do nothing
        else
          sig.class_eval {
            define_method( key ) { node }
          }
        end
      }

      return prg
    end

  end


  class SubProgram < Functor
    attr_reader :words, :faces
    def initialize(*args)
      @words = {}
      @faces = {}
      super
    end
  end


  class Program

    extend Compiler

    class << self
      def words() @words ||= {} end
      def faces() @faces ||= {} end
    end

    ### Instance ###

    attr_reader :stack
    attr_accessor :mode             # defines methods mode() and mode = ...

    # handles setup after constructor
    def initialize( mode = "dec", stack = [] )
      @mode = mode
      @stack = stack
    end

#     # parses and executes expressions in postfix notation
#     # also understands "def OP RUBY_PROC_CODE" on it's own line
#     def calc( postfix_exp )
# 
#       #if postfix_exp =~ /^\s*def\s+(\S+)\s+(\{.+\})\s*$/      # define new op
#       #  define_op( $1.downcase, &eval( "proc #{$2}" ) )
# 
#       #else    # ... or process terms
#         terms = postfix_exp.downcase.split(" ")
# 
#         terms.each do |token|
#           case token
#           when /^-?(?:\d[,_\d]*|0[,_0-7]+|0x[,_0-9a-f]+|0b[,_01]+)$/,
#                /^-?\d[,_\d]*\.\d[,_\d]*(?:e-?[,_\d]+)?$/
#             @stack.unshift eval( token.tr(",", "") )
#           else
#             #begin
#               if token =~ /\w+[.]\w+/
#                 r = eval( token )
#               else
#                 r = send( token )
#               end
#               @stack.unshift r unless r.nil?
#             #rescue
#             #  raise "Invalid term -- '#{token}'"
#             #end
#           end
#         end
#       #end
# 
#       return @stack.first #top
#     end

    ### Operator support processing methods ###

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

    # primary input method, adds something to stack
    #def push( obj )
    #  stack.unshift( obj )
    #end

    #def iface
    #  @iface ||= {}
    #end

    def check( n=1 )
      if @stack.size < n
        raise StackEmptyError, "Insufficient elements on stack for operation."
      end
    end

    def norm( obj )
      if obj.kind_of? Numeric      # don't touch non-numbers
        # the following if converts floats to ints, when it doesn't matter
        if obj.kind_of?(Float) and number == number.to_i
          obj.to_i
        else
          obj
        end
      else
        obj
      end
    end

  end

end
