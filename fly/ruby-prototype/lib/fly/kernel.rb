
module Fly

  # Type type

  class Type
    YAML::add_private_type( 'type' ) { |type,val| Type.new(val) }

    attr_accessor :name, :template

    def intialize( str )
      @template = parse( str )
    end

    def parse( str )
      # tie in kwalify system here
      str
    end

    def compile( name )
      @name = name
      YAML::add_private_type( name ) { |type,val| val } #Type.new(val) }
      @name
    end
  end

  #

  class Fn < String
    ::YAML::add_private_type( 'fn' ) { |type,val| Fn.new( val ) }
  end

  #

  class RubyFn < String
    ::YAML::add_private_type( 'ruby' ) { |type,val| RubyFn.new( val ) }
  end


  class Node

    attr_reader :name, :face, :type, :parent
    attr_accessor :value  # b/c of parental reference

    def initialize( name, face, type, value, parent )
      @name = name
      @face = face
      @type = type
      @value = value
      @parent = parent
    end

    def lookup( name, stack_for_face )
      return self if @name == name #and @face = face
      prt, hit = self.parent, nil
      while prt and not hit
        hit = prt.value.lookup( name, stack_for_face )
        prt = prt.parent
      end
      hit ||= Kernel.model.lookup( name, stack_for_face ) unless prt
      return hit
    end

    def inspect
      if @type == :sub
        "<Node #{name}>"
      else
        super
      end
    end

  end


  class Model

    attr_reader :words, :faces

    def initialize
      @words = Hash.new { |hash, key| hash[key] = {} }
      @faces = Hash.new { |hash, key| hash[key] = [] }
    end

    def new_word( name, face, node)
      words[name][face] ||= node
      faces[name] << face
      faces[name].sort! { |a,b| a.size <=> b.size }
    end

    def start
      words['start'][[]]
    end

#     def lookup( name, stack_for_face )
#       @words[name][[]] if @words.key?(name)
#     end

    def lookup( name, stack )
      return nil unless @words.key?( name )
      fac = @faces[name].dup

      while fac.size > 1
        fac = fac.select{ |f| stack[i].class <= f[-i] }
        i += 1
      end

      if fac[0]
        hit = @words[name][fac[0]]
      elsif @faces[name].key?([])
        hit = @words[name][[]]
      end

      return hit
    end

  end


  # Define kernel methods

  module Kernel

    def self.model
      @model ||= Model.new
    end

    def self.word( name, face='', &block )
      name = name.to_s
      meth = name

      node = Node.new( name, face, :ruby, meth, nil )
      model.new_word( name, face, node )

      define_method( meth, &block )
    end

    ### STACK OPERATIONS ###

    # get top of stack
    word :top do
      @stack.first
    end

    # remove top of stack (returns nil if empty)
    word :drop do
      @stack.shift
    end

    # remove top of stack (returns nil if empty)
    #--
    # NOTE could there be location objects (eg. @5) that regular drop
    #       could see is on the stack and use instead?
    #++
    word :drop_at, 'i' do
      i = pop;
      check i;
      @stack.delete_at( i )
    end

    # drop all elements from index on
    word :drop_from, 'i' do
      i = pop;
      #check i;
      @stack.slice!(0,i)
    end

    # remove top of stack (raises error if empty)
    #pop: !!ruby
    #  check
    #  @stack.shift

    # empty stack
    word :clear do
      @stack.clear
    end

    # duplicate the top of the stack
    word :dup do
      #return unary do |num|
      #  push( num )
      #  num
      #end
      check;
      push @stack.first
    end

    # swap top two elements from stack
    word :swap do
      #return binary do |l, r|
      #  push( r )
      #  l
      #end
      #r, l = @stack.slice!(0,2)
      check 2;
      @stack[0], stack[1] = stack[1], stack[0]
    end

    # dup second item
    word :over do
      push @stack[1].dup
    end

    # arbitrary stack position swap (up)
    word :roll do
      push drop_at( i )
    end

    # arbitrary stack position swap (down)
    word :rolld do
      i = pop;
      o = pop;
      check i;
      @stack.insert(i, o)
    end

    ### BASIC MATH ###

    word '+', 'n n' do
      drop + drop
    end

    word '*', 'n n' do
      drop * drop
    end

    word '-', 'n n' do
      a,b = drop,drop; b - a
    end

    word '%', 'n n' do
      a,b = drop,drop; b % a
    end

    word '**', 'n n' do
      a,b = drop,drop; b ** a
    end

    word '/', 'n n' do
      a,b = drop,drop;
      # defeat Ruby's integer division
      if a.integer? and b.integer? and b % a != 0;
        b / a.to_f;
      else
        b / a;
      end
    end

#     word :floor, 'n' do push drop.floor         end
#     word :ceil,  'n' do push drop.ceil          end
#     word :round, 'n' do push drop.round         end
#
#     word :abs,   'n' do push drop.abs           end
#     word :sqrt,  'n' do push Math.sqrt( drop )  end
#
#     word :exp,   'n' do push Math.exp( drop )   end
#     word :log,   'n' do push Math.log( drop )   end
#     word :log10, 'n' do push Math.log10( drop ) end
#
#     word :sin,   'n' do push Math.sin( drop )   end
#     word :cos,   'n' do push Math.cos( drop )   end
#     word :tan,   'n' do push Math.tan( drop )   end
#     word :sinh,  'n' do push Math.sinh( drop )  end
#     word :cosh,  'n' do push Math.cosh( drop )  end
#     word :tanh,  'n' do push Math.tanh( drop )  end
#     word :asin,  'n' do push Math.asin( drop )  end
#     word :acos,  'n' do push Math.acos( drop )  end
#     word :atan,  'n' do push Math.atan( drop )  end
#     word :sinh,  'n' do push Math.asinh( drop ) end
#     word :acosh, 'n' do push Math.acosh( drop ) end
#     word :atanh, 'n' do push Math.atanh( drop ) end
#
#     word :atan2,  'n n' do
#       a,b = drop,drop;
#       Math.atan2( b, a )
#     end

  end

end #module FLY
