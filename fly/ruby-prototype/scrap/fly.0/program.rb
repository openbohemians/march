#!/usr/bin/env ruby

require 'calibre/functor'
#require 'calibre/overload'

require 'fly/word'
require 'fly/kernel'

# Thanks goes to James Edward Gray II for RPNCalc which got me started on a prototype.

# TODO Allow anchors to work as global variables. How?
#
# TODO Type definitions (Partially implemented)
#
# TODO Implement polymorphic word interface.


module Fly

  CORE_DIR = File.join( Config::CONFIG['datadir'], 'fly', 'core' )
  CORE_PARTS = [ 'math', 'console']



  class Program

    extend Compiler

    ### Instance ###

    attr_reader :stack
    attr_reader :words, :faces

    attr_accessor :mode             # defines methods mode() and mode = ...

    # handles setup after constructor
    def initialize()
      @mode = 'dec'
      @stack = []

      @words = Hash.new { |hash, key| hash[key] = {} }
      @faces = Hash.new { |hash, key| hash[key] = [] }
    end

    def start ; calc( 'start' ) ; end

    # Parses and executes expressions in postfix notation

    def calc( code ) #, *args )

      locals = {}

      terms = code.downcase.split(" ")  # must imporve to handle strings and blocks

      terms.each do |token|

        r = nil

        case token

        when /^-?(?:\d[,_\d]*|0[,_0-7]+|0x[,_0-9a-f]+|0b[,_01]+)$/,
              /^-?\d[,_\d]*\.\d[,_\d]*(?:e-?[,_\d]+)?$/
          r = instance_eval( token.tr(",", "") )

        when /^\'/
          locals[token] = stack.shift

        when /\'$/
          r = locals[token]

        when /^\.$/
          puts drop

        when /\./
          parts = token.split('.')
          #if @parent
          #r = instance_eval( token )

        else
          wrd = match_word( token )
          if wrd
            calc( wrd.code )
          else
            #begin
              Kernel.words( token )
            #rescue
            #  puts "No method error -- #{token}"
            #  exit 1
            #end
          end

        end

        puts "[#{stack.reverse.join(' ')}]: #{token}" #if $DEBUG

        stack.unshift r unless r.nil?

      end

      stack.first
    end



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

  #

  class SubProgram < Program

    def initialize( parent )
      @parent = parent

      @words = {}
      @faces = {}

      super
    end

    def words
      @words
    end

    def faces
      @faces
    end

    def stack; @parent.stack ; end
    def mode; @parent.mode ; end

  end

end
